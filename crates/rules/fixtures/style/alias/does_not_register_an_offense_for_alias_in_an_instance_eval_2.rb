module M
  def foo
    instance_eval {
      alias bar baz
    }
  end
end
