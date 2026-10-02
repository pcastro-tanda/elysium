validates :email, format: { with: /regexp/, allow_blank: true }, presence: { allow_nil: true }
