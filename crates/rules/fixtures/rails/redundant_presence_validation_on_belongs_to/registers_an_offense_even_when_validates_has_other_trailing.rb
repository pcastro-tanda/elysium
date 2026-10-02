belongs_to :user
validates :user, presence: true, uniqueness: true
                 ^^^^^^^^^^^^^^ Remove explicit presence validation for `user`.
