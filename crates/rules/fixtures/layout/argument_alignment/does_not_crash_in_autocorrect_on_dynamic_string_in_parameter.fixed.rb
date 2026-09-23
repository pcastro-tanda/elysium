class MyModel < ActiveRecord::Base
  has_many :other_models,
           class_name: "legacy_name",
           order: "#{legacy_name.table_name}.published DESC"

end
