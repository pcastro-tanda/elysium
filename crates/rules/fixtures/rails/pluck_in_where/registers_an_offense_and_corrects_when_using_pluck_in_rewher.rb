Post.rewhere('user_id IN (?)', User.active.pluck(:id))
                                           ^^^^^ Use `select` instead of `pluck` within `where` query method.
