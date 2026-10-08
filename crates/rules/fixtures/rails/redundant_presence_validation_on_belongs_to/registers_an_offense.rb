belongs_to :user
validates :user, presence: true
                 ^^^^^^^^^^^^^^ Remove explicit presence validation for `user`.
