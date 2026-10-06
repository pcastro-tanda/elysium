Post.where(user_id: User.ids.map(&:to_i))
