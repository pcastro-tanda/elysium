ActiveRecord::Schema.define(version: 2020_02_02_075409) do
  create_table 'emails', force: :cascade do |t|
    t.string 'address', null: false
    t.index 'lower(unexpected_column_name)', name: 'index_emails_on_lower_address', unique: true
  end
end
