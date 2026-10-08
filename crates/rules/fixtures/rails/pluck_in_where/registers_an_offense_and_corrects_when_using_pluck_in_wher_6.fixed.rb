Post&.where(user_id: User&.active&.select(:id))
