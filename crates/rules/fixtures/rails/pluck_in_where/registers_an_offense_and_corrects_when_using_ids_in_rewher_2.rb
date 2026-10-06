Post.rewhere('user_id IN (?)', User.active.ids)
                                           ^^^ Use `select(:id)` instead of `ids` within `where` query method.
