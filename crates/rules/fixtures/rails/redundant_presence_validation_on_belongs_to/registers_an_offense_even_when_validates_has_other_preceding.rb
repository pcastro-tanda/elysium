belongs_to :user
validates :user, uniqueness: true, presence: true
                                   ^^^^^^^^^^^^^^ Remove explicit presence validation for `user`.
