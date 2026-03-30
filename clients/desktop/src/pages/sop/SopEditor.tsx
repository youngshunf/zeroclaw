import React, { useState, useEffect } from 'react';

interface SopEditorProps {
  agentName: string;
  sopName: string;
}

export default function SopEditor({ agentName, sopName }: SopEditorProps) {
  return (
    <div className="flex h-full w-full flex-col">
      <h2 className="text-xl font-bold mb-4">SOP Editor: {sopName}</h2>
      <p>Editing SOP for {agentName}...</p>
    </div>
  );
}
