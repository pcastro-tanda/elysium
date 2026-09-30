scope :bar, lambda { joins(:baz)
                     .distinct }
