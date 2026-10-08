validates :title, length: { is: 5 }, allow_blank: false, allow_nil: false
                                                         ^^^^^^^^^^^^^^^^ `allow_nil` is redundant when `allow_blank` has the same value.
