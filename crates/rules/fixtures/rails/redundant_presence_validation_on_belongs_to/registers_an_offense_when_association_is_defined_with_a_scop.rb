belongs_to :user, -> { not_deleted }
validates :user, presence: true
                 ^^^^^^^^^^^^^^ Remove explicit presence validation for `user`.
