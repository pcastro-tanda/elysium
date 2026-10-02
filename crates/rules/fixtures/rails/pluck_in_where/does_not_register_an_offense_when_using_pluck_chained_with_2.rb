Post.where(user_id: User.pluck(:id).map(&:to_i))
