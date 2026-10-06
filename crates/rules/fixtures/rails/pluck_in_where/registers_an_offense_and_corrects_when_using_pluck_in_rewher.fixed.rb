Post.rewhere('user_id IN (?)', User.active.select(:id))
