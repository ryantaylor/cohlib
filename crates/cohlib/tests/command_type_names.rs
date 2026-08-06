//! Pins every wire name `CommandType` produces.
//!
//! `CommandType`'s `Display` impl delegates to `{:?}` rather than a hand-maintained
//! match table (see the comment on that impl in `crates/replay/src/command_type.rs`),
//! on the basis that the derived variant identifiers already *are* the verbatim Relic
//! wire names. These strings are persisted verbatim by downstream consumers (the
//! `:action_type` key in `CohLib::Command#to_h`), so a rename to a variant — or a
//! change in how `Debug` formats a derived enum — must be a deliberate, visible diff
//! here rather than a silent change to already-stored data.

use cohlib::CommandType;
use std::collections::HashSet;

/// Every mapped `CommandType` name, indexed by its wire byte (`WIRE_NAMES[n]` is the
/// name of the command type whose `id()` is `n`).
const WIRE_NAMES: [&str; 159] = [
    "CMD_DefaultAction",
    "CMD_Stop",
    "CMD_Destroy",
    "CMD_BuildSquad",
    "CMD_InstantBuildSquad",
    "CMD_CancelProduction",
    "CMD_BuildStructure",
    "CMD_Move",
    "CMD_FlightMove",
    "CMD_Face",
    "CMD_Attack",
    "CMD_AttackMove",
    "CMD_RallyPoint",
    "CMD_Capture",
    "CMD_Ability",
    "CMD_Evacuate",
    "CMD_Upgrade",
    "CMD_InstantUpgrade",
    "CMD_Load",
    "CMD_Unload",
    "CMD_UnloadSquads",
    "CMD_AttackStop",
    "CMD_AttackForced",
    "CMD_SetHoldHeading",
    "CMD_StopMove",
    "CMD_Paradrop",
    "CMD_DefuseMine",
    "CMD_Casualty",
    "CMD_Death",
    "CMD_InstantDeath",
    "CMD_Projectile",
    "CMD_PlaceCharge",
    "CMD_BuildEntity",
    "CMD_RescueCasualty",
    "CMD_AttackFromHold",
    "CMD_Vault",
    "CMD_KnockedBack",
    "CMD_Teardown",
    "CMD_Melee",
    "CMD_ResolveOverlap",
    "CMD_Stun",
    "CMD_InstantSetupTeamWeapon",
    "CMD_SetupTeamWeapon",
    "CMD_MoveToCover",
    "CMD_Taunted",
    "CMD_Trade",
    "CMD_Brace",
    "CMD_Gather",
    "CMD_PickUpSimItem",
    "CMD_ChangeCombatSlot",
    "CMD_RetreatMove",
    "CMD_StopAbility",
    "CMD_InstantLoad",
    "CMD_RestoreWreck",
    "CMD_Disable",
    "CMD_Enable",
    "CMD_CancelConstruction",
    "CMD_HoldPositionOn",
    "CMD_HoldPositionOff",
    "CMD_CancelRestoreWreck",
    "CMD_Repair",
    "CMD_COUNT",
    "SCMD_Move",
    "SCMD_Stop",
    "SCMD_Destroy",
    "SCMD_BuildStructure",
    "SCMD_Capture",
    "SCMD_Attack",
    "SCMD_ReinforceUnit",
    "SCMD_Upgrade",
    "SCMD_CancelProduction",
    "SCMD_AttackMove",
    "SCMD_Ability",
    "SCMD_Load",
    "SCMD_InstantLoad",
    "SCMD_UnloadSquads",
    "SCMD_Unload",
    "SCMD_PickupTrailer_UNUSED",
    "SCMD_Retreat",
    "SCMD_CaptureTeamWeapon",
    "SCMD_SetMoveType",
    "SCMD_InstantReinforceUnit",
    "SCMD_InstantUpgrade",
    "SCMD_PlaceCharge",
    "SCMD_DefuseCharge",
    "SCMD_DropTrailer_UNUSED",
    "SCMD_PickUpSimItem",
    "SCMD_DefuseMine",
    "SCMD_DoPlan",
    "SCMD_Patrol",
    "SCMD_Surprise",
    "SCMD_InstantSetupTeamWeapon",
    "SCMD_SetupTeamWeapon",
    "SCMD_AbandonTeamWeapon",
    "SCMD_StationaryAttack",
    "SCMD_RevertFieldSupport",
    "SCMD_Face",
    "SCMD_BuildSquad",
    "SCMD_RallyPoint",
    "SCMD_RescueCasualty",
    "SCMD_Recrew",
    "SCMD_Merge",
    "SCMD_WeaponPreference",
    "SCMD_CombatStance",
    "SCMD_MoveToCover",
    "SCMD_Gather",
    "SCMD_AttackWithinLeashArea",
    "SCMD_JoinFormationSquadGroup",
    "SCMD_Trade",
    "SCMD_HoldPosition",
    "SCMD_Evacuate",
    "SCMD_Vault",
    "SCMD_CancelQueuedCommand",
    "SCMD_RespondToBeingBreached",
    "SCMD_StopAbility",
    "SCMD_InstantParadropReinforceUnit",
    "SCMD_MoveUntilInsidePlayableArea",
    "SCMD_BeingTowed",
    "SCMD_AttachingTrailer",
    "SCMD_DetachingTrailer_UNUSED",
    "SCMD_RestoreWreck_UNUSED",
    "SCMD_AnimatedSpawn",
    "SCMD_COUNT",
    "FCMD_FormationSquadGroupMove",
    "FCMD_FormationSquadGroupAttack",
    "FCMD_FormationSquadGroupAttackMove",
    "FCMD_FormationSquadGroupStop",
    "FCMD_COUNT",
    "PCMD_PlaceAndConstructEntities",
    "PCMD_ResourceDonation",
    "PCMD_CheatResources",
    "PCMD_CheatRevealAll",
    "PCMD_Ability",
    "PCMD_CheatBuildTime",
    "PCMD_CheatIgnoreCosts",
    "PCMD_Upgrade",
    "PCMD_InstantUpgrade",
    "PCMD_TentativeUpgrade",
    "PCMD_TentativeUpgradePurchaseAll",
    "PCMD_UpgradeRemove",
    "PCMD_TentativeUpgradeRemoveAll",
    "PCMD_SlotItemRemove_DEPRECATED",
    "PCMD_CancelProduction",
    "PCMD_DetonateCharges",
    "PCMD_AIPlayer",
    "PCMD_AIPlayer_EncounterNotification",
    "PCMD_Surrender",
    "PCMD_WaitObjectDone",
    "PCMD_BroadcastMessage",
    "PCMD_AIPlayer_EncounterSniped",
    "PCMD_AIPlayer_ResourceBonus",
    "PCMD_FormationSquadGroupCreateBegin",
    "PCMD_FormationSquadGroupAddSquad",
    "PCMD_FormationSquadGroupCreateEnd",
    "PCMD_EndTurn",
    "PCMD_StopAbility",
    "PCMD_COUNT",
    "DCMD_CameraTrack",
    "DCMD_COUNT",
];

#[test]
fn command_type_names_are_stable() {
    for (byte, expected) in WIRE_NAMES.iter().enumerate() {
        let command_type = CommandType::from(byte as u8);
        assert_eq!(&command_type.to_string(), expected, "wire byte {byte}");
        assert_eq!(
            command_type.id(),
            byte as u8,
            "id must round-trip {expected}"
        );
    }
}

#[test]
fn command_type_names_are_unique() {
    let unique: HashSet<_> = WIRE_NAMES.iter().collect();
    assert_eq!(
        unique.len(),
        WIRE_NAMES.len(),
        "duplicate wire name in table"
    );
}

/// Bytes above the known range must be legible and collision-free with mapped names.
#[test]
fn unmapped_command_types_name_their_raw_byte() {
    assert_eq!(CommandType::from(200).to_string(), "UNKNOWN_200");
    assert_eq!(CommandType::from(255).to_string(), "UNKNOWN_255");
}
