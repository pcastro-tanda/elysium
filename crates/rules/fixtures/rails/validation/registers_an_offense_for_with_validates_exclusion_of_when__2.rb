validates_exclusion_of [:full_name, :birth_date].freeze
^^^^^^^^^^^^^^^^^^^^^^ Prefer the new style validations `validates :column, exclusion: value` over `validates_exclusion_of`.
