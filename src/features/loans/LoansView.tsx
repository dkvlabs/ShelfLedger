import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus, Undo2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { api } from "@/lib/api";
import { formatDate, todayIso } from "@/lib/format";
import type { LoanDto } from "@/types/domain";

function LoanRow({ loan }: { loan: LoanDto }) {
  const queryClient = useQueryClient();
  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["loans"] });
    void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
    void queryClient.invalidateQueries({ queryKey: ["work"] });
  };
  const returnLoan = useMutation({
    mutationFn: () => api.returnLoan(loan.id, { returnedAt: todayIso() }),
    onSuccess: invalidate,
  });
  return (
    <li className="flex items-center justify-between rounded-md border px-3 py-2 text-sm">
      <div>
        <div className="font-medium">{loan.workTitle}</div>
        <div className="text-xs text-muted-foreground">
          {loan.contactName} · lent {formatDate(loan.lentAt)}
          {loan.dueAt ? ` · due ${formatDate(loan.dueAt)}` : ""}
          {loan.returnedAt ? ` · returned ${formatDate(loan.returnedAt)}` : ""}
        </div>
      </div>
      <div className="flex items-center gap-2">
        {loan.returnedAt ? (
          <Badge variant="secondary">Returned</Badge>
        ) : loan.overdue ? (
          <Badge variant="destructive">Overdue</Badge>
        ) : (
          <Badge variant="outline">Out</Badge>
        )}
        {!loan.returnedAt && (
          <Button size="sm" variant="outline" onClick={() => returnLoan.mutate()}>
            <Undo2 /> Return
          </Button>
        )}
      </div>
    </li>
  );
}

export default function LoansView() {
  const queryClient = useQueryClient();
  const today = todayIso();
  const [contactName, setContactName] = useState("");

  const { data: loans, isPending } = useQuery({
    queryKey: ["loans", today],
    queryFn: () => api.listLoans(today),
  });
  const { data: contacts } = useQuery({ queryKey: ["contacts"], queryFn: api.listContacts });

  const addContact = useMutation({
    mutationFn: () => api.createContact({ displayName: contactName }),
    onSuccess: () => {
      setContactName("");
      void queryClient.invalidateQueries({ queryKey: ["contacts"] });
    },
  });

  const overdue = (loans ?? []).filter((l) => !l.returnedAt && l.overdue);
  const active = (loans ?? []).filter((l) => !l.returnedAt && !l.overdue);
  const returned = (loans ?? []).filter((l) => l.returnedAt);

  return (
    <div className="max-w-3xl space-y-6 p-6">
      <h1 className="text-xl font-semibold">Loans</h1>
      <p className="text-sm text-muted-foreground">
        Lend a copy from its page in My Library. Contact details stay on this computer;
        nothing is ever sent automatically.
      </p>

      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-sm">Active ({active.length})</CardTitle>
        </CardHeader>
        <CardContent>
          {active.length === 0 ? (
            <p className="text-sm text-muted-foreground">Nothing is out right now.</p>
          ) : (
            <ul className="space-y-2">
              {active.map((l) => (
                <LoanRow key={l.id} loan={l} />
              ))}
            </ul>
          )}
        </CardContent>
      </Card>

      {overdue.length > 0 && (
        <Card className="border-destructive/50">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-destructive">
              Overdue ({overdue.length})
            </CardTitle>
          </CardHeader>
          <CardContent>
            <ul className="space-y-2">
              {overdue.map((l) => (
                <LoanRow key={l.id} loan={l} />
              ))}
            </ul>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-sm">Returned history ({returned.length})</CardTitle>
        </CardHeader>
        <CardContent>
          {returned.length === 0 ? (
            <p className="text-sm text-muted-foreground">No returned loans yet.</p>
          ) : (
            <ul className="space-y-2">
              {returned.map((l) => (
                <LoanRow key={l.id} loan={l} />
              ))}
            </ul>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-sm">Contacts ({contacts?.length ?? 0})</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <ul className="space-y-1 text-sm">
            {(contacts ?? []).map((c) => (
              <li key={c.id} className="rounded-md border px-3 py-2">
                {c.displayName}
                {c.contactInfo && (
                  <span className="text-muted-foreground"> — {c.contactInfo}</span>
                )}
              </li>
            ))}
            {contacts && contacts.length === 0 && (
              <li className="text-muted-foreground">No contacts yet.</li>
            )}
          </ul>
          <div className="flex items-end gap-2">
            <div className="w-64 space-y-1">
              <Label htmlFor="contact-name">New contact</Label>
              <Input
                id="contact-name"
                value={contactName}
                onChange={(e) => setContactName(e.target.value)}
              />
            </div>
            <Button
              disabled={!contactName.trim() || addContact.isPending}
              onClick={() => addContact.mutate()}
            >
              <Plus /> Add
            </Button>
          </div>
        </CardContent>
      </Card>

      {isPending && <p className="text-sm text-muted-foreground">Loading…</p>}
    </div>
  );
}
