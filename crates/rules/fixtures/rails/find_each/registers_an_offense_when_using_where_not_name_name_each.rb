User.where.not(name: name).each { |u| u.something }
                           ^^^^ Use `find_each` instead of `each`.
