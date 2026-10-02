Post.where.not(user_id: User.active.select(:id))
