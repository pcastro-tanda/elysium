User.where(name: name).each { |u| u.something }
                       ^^^^ Use `find_each` instead of `each`.
