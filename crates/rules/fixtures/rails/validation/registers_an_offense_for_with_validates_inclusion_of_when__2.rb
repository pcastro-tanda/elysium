validates_inclusion_of [:full_name, :birth_date].freeze
^^^^^^^^^^^^^^^^^^^^^^ Prefer the new style validations `validates :column, inclusion: value` over `validates_inclusion_of`.
