patch :update,
      id: @user.id,
      ^^^^^^^^^^^^^ Use keyword arguments instead of positional arguments for http call: `patch`.
      ac: {
        article_id: @article1.id,
        profile_id: @profile1.id,
        content: 'Some Text'
      }
