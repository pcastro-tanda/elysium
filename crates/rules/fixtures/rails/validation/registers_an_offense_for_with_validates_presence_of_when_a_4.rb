validates_presence_of %i[full_name birth_date].freeze
^^^^^^^^^^^^^^^^^^^^^ Prefer the new style validations `validates :column, presence: value` over `validates_presence_of`.
