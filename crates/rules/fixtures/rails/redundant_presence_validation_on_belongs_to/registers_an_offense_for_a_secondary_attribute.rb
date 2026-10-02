belongs_to :user
validates :name, :user, presence: true
                        ^^^^^^^^^^^^^^ Remove explicit presence validation for `user`.
