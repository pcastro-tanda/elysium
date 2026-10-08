class Foo
  begin
    begin
      private def do_internal_work; end
      public def do_something; end
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `public_methods` is supposed to appear before `private_methods`.
    end
  end
end
