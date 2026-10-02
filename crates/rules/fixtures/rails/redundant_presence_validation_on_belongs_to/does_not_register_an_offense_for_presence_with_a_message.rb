belongs_to :user
validates :user, presence: { message: 'Must be present' }
