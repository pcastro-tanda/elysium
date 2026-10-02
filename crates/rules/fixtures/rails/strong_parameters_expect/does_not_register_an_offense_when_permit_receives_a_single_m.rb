params.require(:moderation_comment).permit(policy(ModerationComment).permitted_attributes_for_create)
