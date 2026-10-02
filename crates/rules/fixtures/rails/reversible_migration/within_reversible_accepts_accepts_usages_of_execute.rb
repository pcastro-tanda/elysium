class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    reversible do |dir|
  dir.up do
    execute "ALTER TABLE `pages_linked_pages` ADD UNIQUE `page_id_linked_page_id` (`page_id`,`linked_page_id`)"
  end

  dir.down do
    execute "ALTER TABLE `pages_linked_pages` DROP INDEX `page_id_linked_page_id`"
  end
end

  end
end
