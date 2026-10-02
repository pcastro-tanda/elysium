validates :title, allow_nil: false, allow_blank: true, length: { is: 5 }
                  ^^^^^^^^^^^^^^^^ `allow_nil: false` is redundant when `allow_blank` is true.
