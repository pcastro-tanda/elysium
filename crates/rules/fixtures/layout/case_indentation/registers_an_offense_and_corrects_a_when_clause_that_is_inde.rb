case a
    when 0 then return
    ^^^^ Indent `when` as deep as `case`.
    else
        case b
         when 1 then return
         ^^^^ Indent `when` as deep as `case`.
        end
end
