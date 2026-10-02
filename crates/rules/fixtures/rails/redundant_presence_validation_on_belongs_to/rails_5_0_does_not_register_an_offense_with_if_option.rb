belongs_to :user
validates :user, presence: true, if: -> { condition }
