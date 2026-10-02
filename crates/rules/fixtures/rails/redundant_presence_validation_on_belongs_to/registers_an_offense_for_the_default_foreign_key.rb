belongs_to :user
validates :user_id, presence: true
                    ^^^^^^^^^^^^^^ Remove explicit presence validation for `user_id`.
