Foo.where(id: Bar.select(:id)
                        .joins(:bar))
                        ^^^^^^ Align `.joins` with `.select` on line 1.
