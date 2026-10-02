validates :email, format: { with: /regexp/, allow_nil: true, allow_blank: true }
                                            ^^^^^^^^^^^^^^^ `allow_nil` is redundant when `allow_blank` has the same value.
