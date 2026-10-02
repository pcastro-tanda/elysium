Post.where(user_id: users.active.pluck(:id))
                                 ^^^^^ Use `select` instead of `pluck` within `where` query method.
