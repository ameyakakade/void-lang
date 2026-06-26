use std::{borrow::Cow, io::Write};

use fxhash::FxHashMap;
use itertools::Itertools;

use crate::{
    codegen::{Backend, Result},
    ir::Instruction,
    lexer,
};

const RUNTIME: &str = r#"
wow
"#;

pub struct X86_64<'a, 'b, W: Write> {
    writer: &'a mut W,
    symbols: &'b FxHashMap<String, Vec<Instruction>>,
    case_counter: usize,
    eval_counter: usize,
}

impl<'a, 'b, W: Write> X86_64<'a, 'b, W> {
    pub fn new(writer: &'a mut W, symbols: &'b FxHashMap<String, Vec<Instruction>>) -> Self {
        Self {
            writer,
            symbols,
            case_counter: 0,
            eval_counter: 0,
        }
    }
}

impl<'a, 'b, W: Write> Backend<'b> for X86_64<'a, 'b, W> {
    fn emit_push_int(&mut self, int: i64) -> Result<()> {
        writeln!(self.writer, "	; PUSHINT {int}")?;
        writeln!(self.writer, "	mov rdi, {int}")?;
        writeln!(self.writer, "	call __heap_make_int")?;
        writeln!(self.writer, "	sub r15, 8")?;
        writeln!(self.writer, "	mov [r15], rax")?;
        writeln!(self.writer)?;
        Ok(())
    }

    fn emit_push_global(&mut self, name: &str, arity: usize) -> Result<()> {
        writeln!(self.writer, "	; PUSHGLOBAL {name}")?;
        writeln!(self.writer, "	mov rdi, {name}")?;
        writeln!(self.writer, "	mov rsi, {arity}")?;
        writeln!(self.writer, "	call __heap_make_global")?;
        writeln!(self.writer, "	sub r15, 8")?;
        writeln!(self.writer, "	mov [r15], rax")?;
        writeln!(self.writer)?;
        Ok(())
    }

    fn emit_alloc(&mut self) -> Result<()> {
        writeln!(self.writer, "	; ALLOC")?;
        writeln!(self.writer, "	mov rdi, 0")?;
        writeln!(self.writer, "	call __heap_make_ind")?;
        writeln!(self.writer, "	sub r15, 8")?;
        writeln!(self.writer, "	mov [r15], rax")?;
        writeln!(self.writer)?;
        Ok(())
    }

    fn emit_push(&mut self, n: usize) -> Result<()> {
        writeln!(self.writer, "	; PUSH {n}")?;
        writeln!(self.writer, "	mov rdi, [r15+{}]", n * 8)?;
        writeln!(self.writer, "	sub r15, 8")?;
        writeln!(self.writer, "	mov [r15], rdi")?;
        writeln!(self.writer)?;
        Ok(())
    }

    fn emit_pop(&mut self, n: usize) -> Result<()> {
        writeln!(self.writer, "	; POP {n}")?;
        writeln!(self.writer, "	add r15, {}", n * 8)?;
        writeln!(self.writer)?;
        Ok(())
    }

    fn emit_slide(&mut self, n: usize) -> Result<()> {
        writeln!(self.writer, "	; SLIDE {n}")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	add r15, {}", n * 8)?;
        writeln!(self.writer, "	mov [r15], rdi")?;
        writeln!(self.writer)?;
        Ok(())
    }

    fn emit_update(&mut self, n: usize) -> Result<()> {
        writeln!(self.writer, "	; UPDATE {n}")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	add r15, 8")?;
        writeln!(self.writer, "	mov rsi, [r15+{}]", n * 8)?;
        writeln!(self.writer, "	mov qword [rsi], __TAG_IND")?;
        writeln!(self.writer, "	mov [rsi+8], rdi")?;
        writeln!(self.writer)?;
        Ok(())
    }

    fn emit_mkap(&mut self) -> Result<()> {
        writeln!(self.writer, "	; MKAP")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	mov rsi, [r15+8]")?;
        writeln!(self.writer, "	add r15, 16")?;
        writeln!(self.writer, "	call __heap_make_app")?;
        writeln!(self.writer, "	sub r15, 8")?;
        writeln!(self.writer, "	mov [r15], rax")?;
        writeln!(self.writer)?;
        Ok(())
    }

    fn emit_pack(&mut self, tag: usize, arity: usize) -> Result<()> {
        writeln!(self.writer, "	; PACK {tag}, {arity}")?;
        if arity == 0 && tag < 16 {
            writeln!(self.writer, "	sub r15, 8")?;
            writeln!(self.writer, "	mov qword [r15], __const_{tag}_0")?;
        } else {
            writeln!(self.writer, "	mov rdi, {tag}")?;
            writeln!(self.writer, "	mov rsi, {arity}")?;
            writeln!(self.writer, "	mov rdx, r15")?;
            writeln!(self.writer, "	call __heap_make_constr")?;
            writeln!(self.writer, "	mov rdi, {}", arity * 8)?;
            writeln!(self.writer, "	add r15, rdi")?;
            writeln!(self.writer, "	sub r15, 8")?;
            writeln!(self.writer, "	mov [r15], rax")?;
        }
        writeln!(self.writer)?;
        Ok(())
    }

    fn emit_unpack(&mut self, n: usize) -> Result<()> {
        writeln!(self.writer, "	; UNPACK {n}")?;
        writeln!(self.writer, "	mov rsi, [r15]")?;
        writeln!(self.writer, "	add r15, 8")?;
        writeln!(self.writer, "	add rsi, 24")?;
        writeln!(self.writer, "	mov rcx, {n}")?;
        writeln!(self.writer, "	mov rdx, {}", n * 8)?;
        writeln!(self.writer, "	sub r15, rdx")?;
        writeln!(self.writer, "	mov rdi, r15")?;
        writeln!(self.writer, "	cld")?;
        writeln!(self.writer, "	rep movsq")?;
        writeln!(self.writer)?;
        Ok(())
    }

    fn emit_case(&mut self, branches: &FxHashMap<usize, Vec<Instruction>>) -> Result<()> {
        let case_label = format!(".case.{}", self.case_counter);
        self.case_counter += 1;

        writeln!(self.writer, "	; CASE")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	mov rdi, [rdi+8]")?;
        writeln!(self.writer, "	jmp [{case_label}+rdi*8]")?;

        writeln!(self.writer, "	{case_label}:")?;
        for (tag, _) in branches {
            writeln!(self.writer, "	dq {case_label}.{tag}")?;
        }
        writeln!(self.writer)?;

        for (tag, insts) in branches {
            writeln!(self.writer, "	{case_label}.{tag}:")?;
            for inst in insts {
                self.emit_instruction(inst)?
            }
            writeln!(self.writer, "	jmp {case_label}.end")?;
        }
        writeln!(self.writer, "	{case_label}.end:")?;

        writeln!(self.writer)?;
        Ok(())
    }

    fn emit_unwind(&mut self) -> Result<()> {
        writeln!(self.writer, "	; __UNWIND")?;
        writeln!(self.writer, "	jmp __unwind")?;
        writeln!(self.writer)?;
        Ok(())
    }

    fn emit_eval(&mut self) -> Result<()> {
        let eval_label = format!(".eval.{}", self.eval_counter);
        self.eval_counter += 1;

        writeln!(self.writer, "	; EVAL")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	cmp rdi, __TAG_INT")?;
        writeln!(self.writer, "	je {eval_label}")?;
        writeln!(self.writer, "	cmp rdi, __TAG_CNST")?;
        writeln!(self.writer, "	je {eval_label}")?;
        writeln!(self.writer, "	mov rdi, {eval_label}")?;
        writeln!(self.writer, "	mov rsi, r15")?;
        writeln!(self.writer, "	add rsi, 8")?;
        writeln!(self.writer, "	sub r14, 16")?;
        writeln!(self.writer, "	mov [r14], rdi")?;
        writeln!(self.writer, "	mov [r14+8], rsi")?;
        writeln!(self.writer, "	jmp __unwind")?;
        writeln!(self.writer, "	{eval_label}:")?;
        writeln!(self.writer)?;
        Ok(())
    }

    fn emit_instruction(&mut self, inst: &Instruction) -> Result<()> {
        match inst {
            Instruction::PushInt(i) => self.emit_push_int(*i),
            Instruction::PushGlobal(name, arity) => {
                self.emit_push_global(&X86_64::<W>::label(name.as_str()), *arity)
            }
            Instruction::Alloc => self.emit_alloc(),
            Instruction::Push(n) => self.emit_push(*n),
            Instruction::Pop(n) => self.emit_pop(*n),
            Instruction::Slide(n) => self.emit_slide(*n),
            Instruction::Update(n) => self.emit_update(*n),
            Instruction::MkAp => self.emit_mkap(),
            Instruction::Pack(tag, arity) => self.emit_pack(*tag, *arity),
            Instruction::Unpack(n) => self.emit_unpack(*n),
            Instruction::Case(branches) => self.emit_case(&branches),
            Instruction::Eval => self.emit_eval(),
            Instruction::Unwind => self.emit_unwind(),
        }
    }

    fn emit_primitives(&mut self) -> Result<()> {
        writeln!(self.writer, "__int_add:")?;
        self.emit_push(1)?;
        self.emit_eval()?;
        self.emit_push(1)?;
        self.emit_eval()?;
        writeln!(self.writer, "	; add")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	mov rsi, [r15+8]")?;
        writeln!(self.writer, "	add r15, 16")?;
        writeln!(self.writer, "	mov rdi, [rdi+8]")?;
        writeln!(self.writer, "	add rdi, [rsi+8]")?;
        writeln!(self.writer, "	call __heap_make_int")?;
        writeln!(self.writer, "	sub r15, 8")?;
        writeln!(self.writer, "	mov [r15], rax")?;
        writeln!(self.writer)?;
        self.emit_update(2)?;
        self.emit_pop(2)?;
        self.emit_unwind()?;

        writeln!(self.writer, "__int_sub:")?;
        self.emit_push(1)?;
        self.emit_eval()?;
        self.emit_push(1)?;
        self.emit_eval()?;
        writeln!(self.writer, "	; sub")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	mov rsi, [r15+8]")?;
        writeln!(self.writer, "	add r15, 16")?;
        writeln!(self.writer, "	mov rdi, [rdi+8]")?;
        writeln!(self.writer, "	sub rdi, [rsi+8]")?;
        writeln!(self.writer, "	call __heap_make_int")?;
        writeln!(self.writer, "	sub r15, 8")?;
        writeln!(self.writer, "	mov [r15], rax")?;
        writeln!(self.writer)?;
        self.emit_update(2)?;
        self.emit_pop(2)?;
        self.emit_unwind()?;

        writeln!(self.writer, "__int_mul:")?;
        self.emit_push(1)?;
        self.emit_eval()?;
        self.emit_push(1)?;
        self.emit_eval()?;
        writeln!(self.writer, "	; mul")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	mov rsi, [r15+8]")?;
        writeln!(self.writer, "	add r15, 16")?;
        writeln!(self.writer, "	mov rdi, [rdi+8]")?;
        writeln!(self.writer, "	imul rdi, [rsi+8]")?;
        writeln!(self.writer, "	call __heap_make_int")?;
        writeln!(self.writer, "	sub r15, 8")?;
        writeln!(self.writer, "	mov [r15], rax")?;
        writeln!(self.writer)?;
        self.emit_update(2)?;
        self.emit_pop(2)?;
        self.emit_unwind()?;

        writeln!(self.writer, "__int_div:")?;
        self.emit_push(1)?;
        self.emit_eval()?;
        self.emit_push(1)?;
        self.emit_eval()?;
        writeln!(self.writer, "	; sub")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	mov rsi, [r15+8]")?;
        writeln!(self.writer, "	add r15, 16")?;
        writeln!(self.writer, "	mov rdi, [rdi+8]")?;
        writeln!(self.writer, "	mov rax, rdi")?;
        writeln!(self.writer, "	cqo")?;
        writeln!(self.writer, "	idiv qword [rsi+8]")?;
        writeln!(self.writer, "	mov rdi, rax")?;
        writeln!(self.writer, "	call __heap_make_int")?;
        writeln!(self.writer, "	sub r15, 8")?;
        writeln!(self.writer, "	mov [r15], rax")?;
        writeln!(self.writer)?;
        self.emit_update(2)?;
        self.emit_pop(2)?;
        self.emit_unwind()?;

        writeln!(self.writer, "__int_mod:")?;
        self.emit_push(1)?;
        self.emit_eval()?;
        self.emit_push(1)?;
        self.emit_eval()?;
        writeln!(self.writer, "	; mod")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	mov rsi, [r15+8]")?;
        writeln!(self.writer, "	add r15, 16")?;
        writeln!(self.writer, "	mov rdi, [rdi+8]")?;
        writeln!(self.writer, "	mov rax, rdi")?;
        writeln!(self.writer, "	cqo")?;
        writeln!(self.writer, "	idiv qword [rsi+8]")?;
        writeln!(self.writer, "	mov rdi, rdx")?;
        writeln!(self.writer, "	call __heap_make_int")?;
        writeln!(self.writer, "	sub r15, 8")?;
        writeln!(self.writer, "	mov [r15], rax")?;
        writeln!(self.writer)?;
        self.emit_update(2)?;
        self.emit_pop(2)?;
        self.emit_unwind()?;

        writeln!(self.writer, "__int_neg:")?;
        self.emit_push(0)?;
        self.emit_eval()?;
        writeln!(self.writer, "	; neg")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	add r15, 8")?;
        writeln!(self.writer, "	mov rdi, [rdi+8]")?;
        writeln!(self.writer, "	neg rdi")?;
        writeln!(self.writer, "	call __heap_make_int")?;
        writeln!(self.writer, "	sub r15, 8")?;
        writeln!(self.writer, "	mov [r15], rax")?;
        writeln!(self.writer)?;
        self.emit_update(1)?;
        self.emit_pop(1)?;
        self.emit_unwind()?;

        writeln!(self.writer, "__int_eq:")?;
        self.emit_push(1)?;
        self.emit_eval()?;
        self.emit_push(1)?;
        self.emit_eval()?;
        writeln!(self.writer, "	; eq")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	mov rsi, [r15+8]")?;
        writeln!(self.writer, "	add r15, 16")?;
        writeln!(self.writer, "	mov rdi, [rdi+8]")?;
        writeln!(self.writer, "	cmp rdi, [rsi+8]")?;
        writeln!(self.writer, "	je .true")?;
        self.emit_pack(0, 0)?;
        writeln!(self.writer, "	jmp .done")?;
        writeln!(self.writer, "	.true:")?;
        self.emit_pack(1, 0)?;
        writeln!(self.writer, "	.done:")?;
        writeln!(self.writer)?;
        self.emit_update(2)?;
        self.emit_pop(2)?;
        self.emit_unwind()?;

        writeln!(self.writer, "__int_ne:")?;
        self.emit_push(1)?;
        self.emit_eval()?;
        self.emit_push(1)?;
        self.emit_eval()?;
        writeln!(self.writer, "	; ne")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	mov rsi, [r15+8]")?;
        writeln!(self.writer, "	add r15, 16")?;
        writeln!(self.writer, "	mov rdi, [rdi+8]")?;
        writeln!(self.writer, "	cmp rdi, [rsi+8]")?;
        writeln!(self.writer, "	jne .true")?;
        self.emit_pack(0, 0)?;
        writeln!(self.writer, "	jmp .done")?;
        writeln!(self.writer, "	.true:")?;
        self.emit_pack(1, 0)?;
        writeln!(self.writer, "	.done:")?;
        writeln!(self.writer)?;
        self.emit_update(2)?;
        self.emit_pop(2)?;
        self.emit_unwind()?;

        writeln!(self.writer, "__int_lt:")?;
        self.emit_push(1)?;
        self.emit_eval()?;
        self.emit_push(1)?;
        self.emit_eval()?;
        writeln!(self.writer, "	; lt")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	mov rsi, [r15+8]")?;
        writeln!(self.writer, "	add r15, 16")?;
        writeln!(self.writer, "	mov rdi, [rdi+8]")?;
        writeln!(self.writer, "	cmp rdi, [rsi+8]")?;
        writeln!(self.writer, "	jl .true")?;
        self.emit_pack(0, 0)?;
        writeln!(self.writer, "	jmp .done")?;
        writeln!(self.writer, "	.true:")?;
        self.emit_pack(1, 0)?;
        writeln!(self.writer, "	.done:")?;
        writeln!(self.writer)?;
        self.emit_update(2)?;
        self.emit_pop(2)?;
        self.emit_unwind()?;

        writeln!(self.writer, "__int_gt:")?;
        self.emit_push(1)?;
        self.emit_eval()?;
        self.emit_push(1)?;
        self.emit_eval()?;
        writeln!(self.writer, "	; gt")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	mov rsi, [r15+8]")?;
        writeln!(self.writer, "	add r15, 16")?;
        writeln!(self.writer, "	mov rdi, [rdi+8]")?;
        writeln!(self.writer, "	cmp rdi, [rsi+8]")?;
        writeln!(self.writer, "	jg .true")?;
        self.emit_pack(0, 0)?;
        writeln!(self.writer, "	jmp .done")?;
        writeln!(self.writer, "	.true:")?;
        self.emit_pack(1, 0)?;
        writeln!(self.writer, "	.done:")?;
        writeln!(self.writer)?;
        self.emit_update(2)?;
        self.emit_pop(2)?;
        self.emit_unwind()?;

        writeln!(self.writer, "__int_le:")?;
        self.emit_push(1)?;
        self.emit_eval()?;
        self.emit_push(1)?;
        self.emit_eval()?;
        writeln!(self.writer, "	; le")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	mov rsi, [r15+8]")?;
        writeln!(self.writer, "	add r15, 16")?;
        writeln!(self.writer, "	mov rdi, [rdi+8]")?;
        writeln!(self.writer, "	cmp rdi, [rsi+8]")?;
        writeln!(self.writer, "	jle .true")?;
        self.emit_pack(0, 0)?;
        writeln!(self.writer, "	jmp .done")?;
        writeln!(self.writer, "	.true:")?;
        self.emit_pack(1, 0)?;
        writeln!(self.writer, "	.done:")?;
        writeln!(self.writer)?;
        self.emit_update(2)?;
        self.emit_pop(2)?;
        self.emit_unwind()?;

        writeln!(self.writer, "__int_ge:")?;
        self.emit_push(1)?;
        self.emit_eval()?;
        self.emit_push(1)?;
        self.emit_eval()?;
        writeln!(self.writer, "	; ge")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	mov rsi, [r15+8]")?;
        writeln!(self.writer, "	add r15, 16")?;
        writeln!(self.writer, "	mov rdi, [rdi+8]")?;
        writeln!(self.writer, "	cmp rdi, [rsi+8]")?;
        writeln!(self.writer, "	jge .true")?;
        self.emit_pack(0, 0)?;
        writeln!(self.writer, "	jmp .done")?;
        writeln!(self.writer, "	.true:")?;
        self.emit_pack(1, 0)?;
        writeln!(self.writer, "	.done:")?;
        writeln!(self.writer)?;
        self.emit_update(2)?;
        self.emit_pop(2)?;
        self.emit_unwind()?;

        writeln!(self.writer, "__println:")?;
        writeln!(self.writer, "	mov rdi, [r15]")?;
        writeln!(self.writer, "	add r15, 8")?;
        writeln!(self.writer, "	call __print")?;
        writeln!(self.writer, "	mov rdi, 10")?;
        writeln!(self.writer, "	call __print_char")?;
        writeln!(self.writer)?;
        self.emit_pack(0, 0)?;
        self.emit_update(0)?;
        self.emit_unwind()?;

        Ok(())
    }

    fn emit(mut self) -> Result<()> {
        writeln!(self.writer, "{RUNTIME}")?;

        self.emit_primitives()?;

        for (symbol, insts) in self.symbols {
            writeln!(self.writer, "{}:", X86_64::<W>::label(symbol.as_str()))?;
            for inst in insts {
                self.emit_instruction(inst)?
            }
        }
        self.writer.flush()?;

        Ok(())
    }

    fn label(symbol: &str) -> Cow<'_, str> {
        let ch = symbol
            .chars()
            .nth(0)
            .expect("symbol should have at least one character");
        if lexer::is_symbol(ch) {
            Cow::Owned(format!(
                "OP_{}",
                symbol
                    .as_bytes()
                    .iter()
                    .map(|b| format!("{b:02X}"))
                    .join("")
            ))
        } else {
            Cow::Borrowed(symbol)
        }
    }
}

pub fn emit<'a, 'b, W: Write>(
    writer: &'a mut W,
    symbols: &'b FxHashMap<String, Vec<Instruction>>,
) -> Result<()> {
    X86_64::new(writer, symbols).emit()
}
