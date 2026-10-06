validates_uniqueness_of %i[full_name birth_date].freeze
^^^^^^^^^^^^^^^^^^^^^^^ Prefer the new style validations `validates :column, uniqueness: value` over `validates_uniqueness_of`.
