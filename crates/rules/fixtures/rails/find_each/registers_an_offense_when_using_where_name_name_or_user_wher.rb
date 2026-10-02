User.where(name: name).or(User.where(age: age)).each { |u| u.something }
                                                ^^^^ Use `find_each` instead of `each`.
