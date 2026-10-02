has_many :accounts, class_name: :Account, foreign_key: :account_id
has_one :account, class_name: :Account
belongs_to :account, class_name: :Account
