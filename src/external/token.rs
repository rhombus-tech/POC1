use wasmlanche::{public, Context, Address, ContractId, ExternalCallContext};
use crate::{
    types::*,
    state::*,
    core::utils::call_args_from_address,
};

pub fn get_token_context(context: &mut Context) -> ExternalCallContext {
    let token_address = context
        .get(TokenContract())
        .expect("state corrupt")
        .expect("token contract not initialized");
    
    context.to_extern(call_args_from_address(token_address))
}

#[public]
pub fn init_token_contract(
    context: &mut Context,
    token_contract_id: ContractId,
    initial_supply: u64,
) {
    ensure_initialized(context);
    assert!(context.actor() == context.contract_address(), "unauthorized");

    // Deploy token contract
    let token_address = context.deploy(token_contract_id, &[]);
    let token_args = call_args_from_address(token_address);
    let token_context = context.to_extern(token_args);

    // Initialize token contract
    token::init(
        token_context,
        String::from("TEE System Token"),
        String::from("TST"),
    );

    // Store token contract address
    context
        .store_by_key(TokenContract(), token_address)
        .expect("failed to store token contract");

    // Mint initial supply to contract
    let mint_context = context.to_extern(call_args_from_address(token_address));
    token::mint(mint_context, context.contract_address(), initial_supply);
}

#[public]
pub fn stake_tokens(context: &mut Context, amount: u64) {
    ensure_initialized(context);
    let caller = context.actor();

    // Verify caller is executor or watchdog
    let executor_pool = context
        .get(ExecutorPool())
        .expect("state corrupt")
        .expect("executor pool not initialized");
    
    let watchdog_pool = context
        .get(WatchdogPool())
        .expect("state corrupt")
        .expect("watchdog pool not initialized");

    let is_executor = executor_pool.sgx_executor == Some(caller) || 
                     executor_pool.sev_executor == Some(caller);
    let is_watchdog = watchdog_pool.watchdogs.iter().any(|(addr, _)| *addr == caller);

    assert!(is_executor || is_watchdog, "unauthorized staker");

    // Transfer tokens from caller to contract
    let token_context = get_token_context(context);
    token::transfer_from(token_context, caller, context.contract_address(), amount);

    // Record stake
    let interaction = TokenInteraction {
        token_address: token_context.contract_address,
        amount,
        interaction_type: TokenInteractionType::Stake,
    };

    record_token_interaction(context, caller, interaction);
}

#[public]
pub fn distribute_rewards(context: &mut Context) {
    ensure_initialized(context);
    ensure_phase(context, Phase::Executing);

    let executor_pool = context
        .get(ExecutorPool())
        .expect("state corrupt")
        .expect("executor pool not initialized");

    let watchdog_pool = context
        .get(WatchdogPool())
        .expect("state corrupt")
        .expect("watchdog pool not initialized");

    let token_context = get_token_context(context);
    let contract_balance = token::balance_of(token_context, context.contract_address());

    // Calculate rewards
    let executor_reward = contract_balance / 3; // 1/3 for executors
    let watchdog_reward = contract_balance / 3; // 1/3 for watchdogs
    // 1/3 remains in contract for future operations

    // Distribute to executors
    if let Some(sgx_executor) = executor_pool.sgx_executor {
        token::transfer(
            token_context,
            sgx_executor,
            executor_reward / 2,
        );
    }
    if let Some(sev_executor) = executor_pool.sev_executor {
        token::transfer(
            token_context,
            sev_executor,
            executor_reward / 2,
        );
    }

    // Distribute to watchdogs
    let watchdog_count = watchdog_pool.watchdogs.len();
    if watchdog_count > 0 {
        let reward_per_watchdog = watchdog_reward / watchdog_count as u64;
        for (watchdog, _) in watchdog_pool.watchdogs {
            token::transfer(
                token_context,
                watchdog,
                reward_per_watchdog,
            );
        }
    }
}

#[public]
pub fn get_token_balance(context: &mut Context, address: Address) -> u64 {
    ensure_initialized(context);
    let token_context = get_token_context(context);
    token::balance_of(token_context, address)
}

#[public]
pub fn get_total_staked(context: &mut Context) -> u64 {
    ensure_initialized(context);
    let token_context = get_token_context(context);
    token::balance_of(token_context, context.contract_address())
}

#[public]
pub fn has_minimum_stake(context: &mut Context, address: Address) -> bool {
    ensure_initialized(context);
    let token_context = get_token_context(context);
    let balance = token::balance_of(token_context, address);
    
    let min_stake = match context.get(EnclaveType(address)) {
        Ok(Some(EnclaveType::IntelSGX)) => 1000,
        Ok(Some(EnclaveType::AMDSEV)) => 1000,
        _ => return false,
    };

    balance >= min_stake
}

fn record_token_interaction(
    context: &mut Context,
    address: Address,
    interaction: TokenInteraction,
) {
    update_global_state(context);
}
