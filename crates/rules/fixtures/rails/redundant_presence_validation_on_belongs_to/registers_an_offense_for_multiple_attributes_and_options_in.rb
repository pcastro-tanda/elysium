belongs_to :user, -> { where(foo: true) }, inverse_of: :employee
validates :user_id, presence: true
                    ^^^^^^^^^^^^^^ Remove explicit presence validation for `user_id`.
