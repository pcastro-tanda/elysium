get some_path(profile.id), params: { user_id: @user.id, profile_id: p.id }, session: { 'HTTP_REFERER' => p_url(p.id).to_s }
