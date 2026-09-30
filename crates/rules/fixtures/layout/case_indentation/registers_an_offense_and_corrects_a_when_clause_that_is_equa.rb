y = case a
    when 0 then raise
    ^^^^ Indent `when` one step more than `case`.
    when 0 then return
    ^^^^ Indent `when` one step more than `case`.
      z = case b
          when 1 then return
          ^^^^ Indent `when` one step more than `case`.
          when 1 then raise
          ^^^^ Indent `when` one step more than `case`.
          end
    end
case c
when 2 then encoding
^^^^ Indent `when` one step more than `case`.
end
