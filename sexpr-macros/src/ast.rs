pub enum Ast {
    Boolean(bool),
    Int(u64),
    Symbol(String),
    Keyword(String),
    String(String),
    List(Vec<Ast>),
    ImproperList(Vec<Ast>, Box<Ast>),
}
