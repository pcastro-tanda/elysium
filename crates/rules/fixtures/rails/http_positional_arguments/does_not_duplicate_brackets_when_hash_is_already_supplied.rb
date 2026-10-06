get some_path(profile.id), { user_id: @user.id, profile_id: p.id }, 'HTTP_REFERER' => p_url(p.id).to_s
                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use keyword arguments instead of positional arguments for http call: `get`.
