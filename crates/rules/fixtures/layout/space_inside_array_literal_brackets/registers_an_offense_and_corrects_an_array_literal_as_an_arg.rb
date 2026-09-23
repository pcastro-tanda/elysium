ActiveRecord::Base.connection.execute(<<-SQL, [self.class.to_s ]).first["count"]
                                                              ^ Do not use space inside array brackets.
  SELECT COUNT(widgets.id) FROM widgets
  WHERE widget_type = $1
SQL
