use serde_derive::Deserialize;
use serde_json::Value;

use self::message_delete::MessageDeleteData;
use crate::receive_struct::call_create::CallCreateData;
use crate::receive_struct::call_delete::CallDeleteData;
use crate::receive_struct::channel_create::ChannelCreateData;
use crate::receive_struct::channel_unread_update::ChannelUnreadUpdate;
use crate::receive_struct::gateway_events::*;
use crate::receive_struct::guild_delete::GuildDelete;
use crate::receive_struct::guild_integration_update::GuildIntegrationsUpdate;
use crate::receive_struct::guild_member_remove::GuildMemberRemove;
use crate::receive_struct::integration_update::IntegrationUpdate;
use crate::receive_struct::message_ack::MessageAck;
use crate::receive_struct::presence_update::PresenceData;
use crate::receive_struct::session_replace::SessionReplaceData;
use crate::receive_struct::typing_start::TypingData;
use crate::receive_struct::user_application_identity_update::UserApplicationIdentityUpdate;
use crate::receive_struct::user_guild_settings_update::UserGuildSettingsUpdateData;
use crate::receive_struct::user_settings_update::UserSettingsUpdateData;
use crate::receive_struct::voice_channel_start_time_update::VoiceChannelStartTimeUpdateData;
use crate::receive_struct::voice_channel_status_update::VoiceChannelStatusUpdateData;
use crate::receive_struct::voice_server_update::VoiceServerUpdateData;
use crate::receive_struct::voice_state_update::VoiceStateUpdateData;
use crate::receive_struct::*;

#[derive(Deserialize, Debug)]
#[serde(tag = "t", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscordEventMessage {
    Ready {
        d: Box<ReadyData>,
    },
    ReadySupplemental {
        d: ReadySupplementalData,
    },
    Resumed {
        d: ResumedData,
    },
    PassiveUpdateV1 {
        d: PassiveUpdateV1Data,
    },

    ApplicationCommandPermissionsUpdate {
        d: ApplicationCommandPermissionsUpdate,
    },
    AutoModerationRuleCreate {
        d: AutoModerationRule,
    },
    AutoModerationRuleUpdate {
        d: AutoModerationRule,
    },
    AutoModerationRuleDelete {
        d: AutoModerationRule,
    },
    AutoModerationRuleExecution {
        d: AutoModerationExecution,
    },

    ChannelCreate {
        d: ChannelCreateData,
    },
    ChannelDelete {
        d: ChannelCreateData,
    },
    ChannelUpdate {
        d: ChannelCreateData,
    },
    ChannelPinsAck {
        d: ChannelPinsAck,
    },
    ChannelPinsUpdate {
        d: ChannelPinsUpdate,
    },
    ChannelTopicUpdate {
        d: ChannelTopicUpdate,
    },
    ChannelUnreadUpdate {
        d: ChannelUnreadUpdate,
    },

    ThreadCreate {
        d: ThreadEvent,
    },
    ThreadUpdate {
        d: ThreadEvent,
    },
    ThreadDelete {
        d: ThreadEvent,
    },
    ThreadMemberUpdate {
        d: ThreadMembersUpdate,
    },
    ThreadMembersUpdate {
        d: ThreadMembersUpdate,
    },
    ThreadListSync {
        d: ThreadListSync,
    },

    EmbeddedActivityUpdateV2 {
        d: EmbeddedActivityUpdateV2,
    },

    EntitlementCreate {
        d: Entitlement,
    },
    EntitlementUpdate {
        d: Entitlement,
    },
    EntitlementDelete {
        d: Entitlement,
    },
    SubscriptionCreate {
        d: Subscription,
    },
    SubscriptionUpdate {
        d: Subscription,
    },
    SubscriptionDelete {
        d: Subscription,
    },

    GuildCreate {
        d: GuildCreate,
    },
    GuildUpdate {
        d: GuildUpdate,
    },
    GuildDelete {
        d: GuildDelete,
    },
    GuildIntegrationsUpdate {
        d: GuildIntegrationsUpdate,
    },
    GuildJoinRequestCreate {
        d: GuildJoinRequest,
    },
    GuildJoinRequestUpdate {
        d: GuildJoinRequest,
    },
    GuildScheduledEventCreate {
        d: GuildScheduledEvent,
    },
    GuildScheduledEventUpdate {
        d: GuildScheduledEvent,
    },
    GuildScheduledEventDelete {
        d: GuildScheduledEvent,
    },
    GuildScheduledEventUserAdd {
        d: GuildScheduledEventUser,
    },
    GuildScheduledEventUserRemove {
        d: GuildScheduledEventUser,
    },
    GuildScheduledEventExceptionsDelete {
        d: Value,
    },
    GuildSoundboardSoundCreate {
        d: GuildSoundboardSound,
    },
    GuildSoundboardSoundUpdate {
        d: GuildSoundboardSound,
    },
    GuildSoundboardSoundDelete {
        d: Value,
    },
    GuildSoundboardSoundsUpdate {
        d: GuildSoundboardSoundsUpdate,
    },
    GuildRoleCreate {
        d: GuildRoleUpsert,
    },
    GuildRoleUpdate {
        d: GuildRoleUpsert,
    },
    GuildRoleDelete {
        d: GuildRoleDelete,
    },
    GuildEmojisUpdate {
        d: GuildEmojisUpdate,
    },
    GuildStickersUpdate {
        d: GuildStickersUpdate,
    },
    GuildMemberAdd {
        d: GuildMemberAdd,
    },
    GuildMemberRemove {
        d: GuildMemberRemove,
    },
    GuildMemberUpdate {
        d: GuildMemberUpdate,
    },
    GuildMemberChunk {
        d: GuildMemberChunk,
    },
    GuildBanAdd {
        d: GuildBanAdd,
    },
    GuildApplicationCommandIndexUpdate {
        d: GuildApplicationCommandIndexUpdate,
    },
    GuildAuditLogEntryCreate {
        d: GuildAuditLogEntryCreate,
    },

    CallCreate {
        d: CallCreateData,
    },
    CallDelete {
        d: CallDeleteData,
    },
    PresenceUpdate {
        d: PresenceData,
    },

    MessageCreate {
        d: MessageCreateData,
    },
    MessageUpdate {
        d: MessageCreateData,
    },
    MessageDelete {
        d: MessageDeleteData,
    },
    MessageReactionAdd {
        d: MessageReactionAdd,
    },
    MessageReactionRemove {
        d: MessageReactionRemove,
    },
    MessageReactionRemoveAll {
        d: MessageReactionRemoveAll,
    },
    MessageReactionRemoveEmoji {
        d: MessageReactionRemoveEmoji,
    },
    MessageDeleteBulk {
        d: MessageDeleteBulk,
    },
    MessageAck {
        d: MessageAck,
    },
    MessagePollVoteAdd {
        d: MessagePollVote,
    },
    MessagePollVoteRemove {
        d: MessagePollVote,
    },

    TypingStart {
        d: TypingData,
    },

    VoiceChannelEffectSend {
        d: VoiceChannelEffectSend,
    },
    VoiceStateUpdate {
        d: VoiceStateUpdateData,
    },
    VoiceServerUpdate {
        d: VoiceServerUpdateData,
    },
    VoiceChannelStatusUpdate {
        d: VoiceChannelStatusUpdateData,
    },
    VoiceChannelStartTimeUpdate {
        d: VoiceChannelStartTimeUpdateData,
    },

    SessionsReplace {
        d: Vec<SessionReplaceData>,
    },

    ConversationSummaryUpdate {
        d: ConversationSummaryUpdate,
    },
    SoundboardSounds {
        d: SoundboardSounds,
    },
    IntegrationCreate {
        d: IntegrationUpdate,
    },
    IntegrationUpdate {
        d: IntegrationUpdate,
    },
    IntegrationDelete {
        d: IntegrationDelete,
    },
    InviteCreate {
        d: InviteCreate,
    },
    InviteDelete {
        d: InviteDelete,
    },

    UserApplicationIdentityUpdate {
        d: UserApplicationIdentityUpdate,
    },
    UserApplicationUpdate {
        d: UserApplicationUpdate,
    },
    UserConnectionsUpdate {
        d: UserConnectionsUpdate,
    },
    UserSettingsProtoUpdate {
        d: UserSettingsProtoUpdate,
    },
    UserGuildSettingsUpdate {
        d: UserGuildSettingsUpdateData,
    },
    UserNoteUpdate {
        d: UserNoteUpdate,
    },
    UserSettingsUpdate {
        d: UserSettingsUpdateData,
    },
    UserUpdate {
        d: UserUpdate,
    },
    UserNonChannelAck {
        d: UserNonChannelAck,
    },

    RelationshipAdd {
        d: RelationshipAdd,
    },
    RelationshipRemove {
        d: RelationshipRemove,
    },

    WebhooksUpdate {
        d: WebhooksUpdate,
    },

    InteractionCreate {
        d: InteractionCreate,
    },
    InteractionSuccess {
        d: InteractionSuccess,
    },

    NotificationCenterItemCreate {
        d: NotificationCenterItem,
    },
    NotificationCenterItemDelete {
        d: NotificationCenterItem,
    },

    RecentMentionDelete {
        d: RecentMentionDelete,
    },

    StageInstanceCreate {
        d: StageInstance,
    },
    StageInstanceUpdate {
        d: StageInstance,
    },
    StageInstanceDelete {
        d: StageInstance,
    },

    ContentInventoryInboxStale {
        d: ContentInventoryInboxStale,
    },

    Oauth2TokenCreate {
        d: Oauth2TokenCreate,
    },

    StreamCreate {
        d: serde_json::Value,
    },
    StreamServerUpdate {
        d: serde_json::Value,
    },
    StreamUpdate {
        d: serde_json::Value,
    },
    StreamDelete {
        d: serde_json::Value,
    },
}
