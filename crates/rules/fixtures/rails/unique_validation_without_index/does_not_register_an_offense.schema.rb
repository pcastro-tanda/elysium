ActiveRecord::Schema.define(version: 2020_02_02_075409) do
  create_table "users", force: :cascade do |t|
    t.string "account", null: false
    t.index ["account"], name: "index_users_on_account", unique: true
    t.check_constraint nil, 'expression', name: "constraint_name"
  end
end
