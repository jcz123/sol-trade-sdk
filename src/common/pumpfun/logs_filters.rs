use crate::common::pumpfun::logs_data::DexInstruction;
use crate::common::pumpfun::logs_parser::{parse_create_token_data, parse_trade_data, parse_instruction_create_token_data, parse_instruction_trade_data};
use crate::error::ClientResult;
pub struct LogFilter;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use solana_sdk::transaction::VersionedTransaction;

impl LogFilter {
    const PROGRAM_ID: &'static str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

    /// Parse transaction logs and return instruction type and data
    pub fn parse_compiled_instruction(
        versioned_tx: VersionedTransaction,
        bot_wallet: Option<Pubkey>) -> ClientResult<Vec<DexInstruction>> {
        let compiled_instructions = versioned_tx.message.instructions(); 
        let accounts = versioned_tx.message.static_account_keys();
        let program_id = Pubkey::from_str(Self::PROGRAM_ID).unwrap_or_default();
        let pump_index = accounts.iter().position(|key| key == &program_id);
        let mut instructions: Vec<DexInstruction> = Vec::new();
        if let Some(index) = pump_index {
            for instruction in compiled_instructions {
                if instruction.program_id_index as usize == index {
                    let all_accounts_valid = instruction.accounts.iter()
                    .all(|&acc_idx| (acc_idx as usize) < accounts.len());
                    if !all_accounts_valid {
                        continue;
                    }
                    match instruction.data.first() {
                        // create
                        Some(&24) => {     
                            if let Ok(token_info) = parse_instruction_create_token_data(instruction, accounts) {
                                instructions.push(DexInstruction::CreateToken(token_info));
                            };
                        }
                        // buy
                        Some(&102) if instruction.data.len() == 24 && instruction.accounts.len() >= 12 => {
                            if let Ok(trade_info) = parse_instruction_trade_data(instruction, accounts, true) {
                                if let Some(bot_wallet_pubkey) = bot_wallet {
                                    if trade_info.user.to_string() == bot_wallet_pubkey.to_string() {
                                        instructions.push(DexInstruction::BotTrade(trade_info));
                                    } else {
                                        instructions.push(DexInstruction::UserTrade(trade_info));
                                    }
                                } else {
                                    instructions.push(DexInstruction::UserTrade(trade_info));
                                }
                            };
                        }
                        // sell
                        Some(&51) if instruction.data.len() == 24 && instruction.accounts.len() >= 12 => {
                            if let Ok(trade_info) = parse_instruction_trade_data(instruction, accounts, false) {
                                if let Some(bot_wallet_pubkey) = bot_wallet {
                                    if trade_info.user.to_string() == bot_wallet_pubkey.to_string() {
                                        instructions.push(DexInstruction::BotTrade(trade_info));
                                    } else {
                                        instructions.push(DexInstruction::UserTrade(trade_info));
                                    }
                                } else {
                                    instructions.push(DexInstruction::UserTrade(trade_info));
                                }
                            };
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(instructions)
    }

    
    /// Parse transaction logs and return instruction type and data
    pub fn parse_instruction(logs: &[String], bot_wallet: Option<Pubkey>) -> ClientResult<Vec<DexInstruction>> {
        let mut current_instruction = None;
        let mut program_data = String::new();
        let mut invoke_depth = 0;
        let mut last_data_len = 0;
        let mut instructions = Vec::new();
        for log in logs {
            // Check program invocation
            if log.contains(&format!("Program {} invoke", Self::PROGRAM_ID)) {
                invoke_depth += 1;
                if invoke_depth == 1 {  // Only reset state at top level call
                    current_instruction = None;
                    program_data.clear();
                    last_data_len = 0;
                }
                continue;
            }
            
            // Skip if not in our program
            if invoke_depth == 0 {
                continue;
            }
            
            // Identify instruction type (only at top level)
            if invoke_depth == 1 && log.contains("Program log: Instruction:") {
                if log.contains("Create") {
                    current_instruction = Some("create");
                } else if log.contains("Buy") || log.contains("Sell") {
                    current_instruction = Some("trade");
                }
                continue;
            }
            
            // Collect Program data
            if log.starts_with("Program data: ") {
                let data = log.trim_start_matches("Program data: ");
                if data.len() > last_data_len {
                    program_data = data.to_string();
                    last_data_len = data.len();
                }
            }
            
            // Check if program ends
            if log.contains(&format!("Program {} success", Self::PROGRAM_ID)) {
                invoke_depth -= 1;
                if invoke_depth == 0 {  // Only process data when top level program ends
                    if let Some(instruction_type) = current_instruction {
                        if !program_data.is_empty() {
                            match instruction_type {
                                "create" => {
                                    if let Ok(token_info) = parse_create_token_data(&program_data) {
                                        instructions.push(DexInstruction::CreateToken(token_info));
                                    }
                                },
                                "trade" => {
                                    if let Ok(trade_info) = parse_trade_data(&program_data) {
                                        if let Some(bot_wallet_pubkey) = bot_wallet {
                                            if trade_info.user.to_string() == bot_wallet_pubkey.to_string() {
                                                instructions.push(DexInstruction::BotTrade(trade_info));
                                            } else {
                                                instructions.push(DexInstruction::UserTrade(trade_info));
                                            }
                                        } else {
                                            instructions.push(DexInstruction::UserTrade(trade_info));
                                        }
                                    }
                                },
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        Ok(instructions)
    }
}