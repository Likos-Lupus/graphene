use graphene_auth::{AccountKind, AuthSession};
use graphene_core::Result;
use graphene_launch::LaunchSession;

pub(super) fn to_launch_session(session: AuthSession) -> Result<LaunchSession> {
    let launch = LaunchSession {
        username: session.profile.display_name,
        uuid: session.profile.minecraft_uuid.to_string(),
        access_token: session.access_token,
        user_type: if session.kind == AccountKind::Microsoft {
            "msa".into()
        } else {
            "legacy".into()
        },
        client_id: session.client_id,
        xuid: session.xuid,
    };
    launch.validate()?;
    Ok(launch)
}
