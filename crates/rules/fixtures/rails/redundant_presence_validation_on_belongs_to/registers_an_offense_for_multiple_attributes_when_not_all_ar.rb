belongs_to :user
validates :user, :name, presence: true
                        ^^^^^^^^^^^^^^ Remove explicit presence validation for `user`.
