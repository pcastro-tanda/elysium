Post.where(user_id: User.active.pluck(:id))
                                ^^^^^ Use `select` instead of `pluck` within `where` query method.
