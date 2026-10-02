Post.where.not(user_id: User.active.ids)
                                    ^^^ Use `select(:id)` instead of `ids` within `where` query method.
