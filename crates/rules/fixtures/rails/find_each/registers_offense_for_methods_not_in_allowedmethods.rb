User.joins(:posts).each { |u| u.something }
                   ^^^^ Use `find_each` instead of `each`.
