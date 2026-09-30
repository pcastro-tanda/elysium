class Test
  def foo
    <<-SQL
      bar
  SQL
^^^^^ `SQL` is not aligned with `<<-SQL`.
  end
end
